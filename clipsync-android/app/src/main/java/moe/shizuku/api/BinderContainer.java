package moe.shizuku.api;

import android.os.IBinder;
import android.os.Parcel;
import android.os.Parcelable;

public class BinderContainer implements Parcelable {
    public final IBinder binder;

    public BinderContainer(IBinder binder) {
        this.binder = binder;
    }

    protected BinderContainer(Parcel in) {
        this.binder = in.readStrongBinder();
    }

    public IBinder getBinder() {
        return this.binder;
    }

    @Override
    public void writeToParcel(Parcel dest, int flags) {
        dest.writeStrongBinder(binder);
    }

    @Override
    public int describeContents() {
        return 0;
    }

    public static final Creator<BinderContainer> CREATOR = new Creator<BinderContainer>() {
        public BinderContainer createFromParcel(Parcel in) {
            return new BinderContainer(in);
        }
        public BinderContainer[] newArray(int size) {
            return new BinderContainer[size];
        }
    };
}
